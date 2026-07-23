package org.hyperledger.iroha.android.crypto;

import org.bouncycastle.math.ec.rfc8032.Ed25519;

/** Strict admission checks for canonical prime-order Ed25519 public keys. */
public final class Ed25519PublicKeyAdmission {

  /** Canonical compressed Ed25519 public-key length. */
  public static final int PUBLIC_KEY_LENGTH = 32;

  private Ed25519PublicKeyAdmission() {}

  /** Returns {@code true} only for canonical points in the prime-order Ed25519 subgroup. */
  public static boolean isValid(final byte[] publicKey) {
    return publicKey != null
        && publicKey.length == PUBLIC_KEY_LENGTH
        && Ed25519.validatePublicKeyFull(publicKey, 0);
  }
}
