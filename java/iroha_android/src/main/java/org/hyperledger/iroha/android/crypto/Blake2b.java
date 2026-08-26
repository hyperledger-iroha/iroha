package org.hyperledger.iroha.android.crypto;

import java.util.Objects;
import org.bouncycastle.crypto.digests.Blake2bDigest;

/** BLAKE2b helpers backed by the SDK's directly linked Bouncy Castle implementation. */
public final class Blake2b {

  private Blake2b() {}

  /** Returns the 256-bit BLAKE2b digest of {@code message}. */
  public static byte[] digest256(final byte[] message) {
    return digest(message, 32);
  }

  /** Returns the 512-bit BLAKE2b digest of {@code message}. */
  public static byte[] digest512(final byte[] message) {
    return digest(message, 64);
  }

  /** Returns the canonical 256-bit BLAKE2b digest of {@code message}. */
  public static byte[] digest(final byte[] message) {
    return digest256(message);
  }

  /** Returns a BLAKE2b digest of {@code message} with exactly {@code outLen} bytes. */
  public static byte[] digest(final byte[] message, final int outLen) {
    Objects.requireNonNull(message, "message");
    if (outLen <= 0 || outLen > 64) {
      throw new IllegalArgumentException("BLAKE2b output length must be between 1 and 64 bytes");
    }

    final Blake2bDigest digest = new Blake2bDigest(outLen * Byte.SIZE);
    digest.update(message, 0, message.length);
    final byte[] output = new byte[outLen];
    digest.doFinal(output, 0);
    return output;
  }
}
