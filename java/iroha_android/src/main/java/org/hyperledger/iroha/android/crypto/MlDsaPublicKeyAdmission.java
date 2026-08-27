package org.hyperledger.iroha.android.crypto;

/** Protocol-shape admission checks for raw ML-DSA-65 public keys. */
public final class MlDsaPublicKeyAdmission {

  /** Canonical raw ML-DSA-65 public-key length. */
  public static final int PUBLIC_KEY_LENGTH = 1952;

  private MlDsaPublicKeyAdmission() {}

  /** Returns {@code true} only for an exact-width, nonzero ML-DSA-65 public key. */
  public static boolean isValid(final byte[] publicKey) {
    if (publicKey == null || publicKey.length != PUBLIC_KEY_LENGTH) {
      return false;
    }
    for (final byte value : publicKey) {
      if (value != 0) {
        return true;
      }
    }
    return false;
  }
}
