package org.hyperledger.iroha.android.crypto;

/** Iroha hashing helpers (Blake2b-256 with prehash marker). */
public final class IrohaHash {

  private IrohaHash() {}

  /**
   * Hashes the provided bytes using Blake2b-256 and sets the least significant bit to 1.
   *
   * @param message bytes to hash (must not be null)
   * @return hashed bytes with prehash marker applied
   */
  public static byte[] prehash(final byte[] message) {
    if (message == null) {
      throw new IllegalArgumentException("message must not be null");
    }
    final byte[] digest = Blake2b.digest256(message);
    digest[digest.length - 1] |= 1;
    return digest;
  }
}
