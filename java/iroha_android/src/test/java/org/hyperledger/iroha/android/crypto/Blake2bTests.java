package org.hyperledger.iroha.android.crypto;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertThrows;

import java.nio.charset.StandardCharsets;
import org.junit.Test;

/** Reference-vector tests for the Java BLAKE2b facade. */
public final class Blake2bTests {

  @Test
  public void fixedAndVariableDigestsMatchReferenceVectors() {
    final byte[] message = "abc".getBytes(StandardCharsets.UTF_8);
    final byte[] digest256 =
        hex("bddd813c634239723171ef3fee98579b94964e3bb1cb3e427262c8c068d52319");
    final byte[] digest512 =
        hex(
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d"
                + "17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923");

    assertArrayEquals(digest256, Blake2b.digest256(message));
    assertArrayEquals(digest256, Blake2b.digest(message));
    assertArrayEquals(digest512, Blake2b.digest512(message));
    assertArrayEquals(hex("d8bb14d833d59559"), Blake2b.digest(message, 8));
  }

  @Test
  public void variableDigestRejectsInvalidOutputLengths() {
    assertThrows(IllegalArgumentException.class, () -> Blake2b.digest(new byte[0], 0));
    assertThrows(IllegalArgumentException.class, () -> Blake2b.digest(new byte[0], 65));
  }

  private static byte[] hex(final String value) {
    final byte[] output = new byte[value.length() / 2];
    for (int index = 0; index < output.length; index++) {
      final int offset = index * 2;
      output[index] = (byte) Integer.parseInt(value.substring(offset, offset + 2), 16);
    }
    return output;
  }
}
