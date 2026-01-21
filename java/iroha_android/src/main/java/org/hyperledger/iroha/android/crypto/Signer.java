package org.hyperledger.iroha.android.crypto;

import org.hyperledger.iroha.android.SigningException;

/** Computes digital signatures for Iroha payloads. */
public interface Signer {

  /**
   * Signs the provided message and returns the encoded signature.
   *
   * <p>The caller must pass the exact bytes to be signed. Iroha transaction helpers
   * apply the Blake2b-256 prehash before invoking this method.
   *
   * @param message payload to sign (must not be null)
   * @return encoded signature bytes
   * @throws SigningException if the signature cannot be produced
   */
  byte[] sign(byte[] message) throws SigningException;

  /**
   * Returns the encoded public key corresponding to this signer.
   *
   * <p>The encoding matches the underlying provider; for Ed25519 this is the X.509 SubjectPublicKeyInfo
   * representation. Future revisions will offer Norito-friendly wrappers.
   */
  byte[] publicKey();

  /**
   * Optional BLS public key used for multi-signature contexts. Implementations that do not support
   * BLS can return {@code null}.
   */
  default byte[] blsPublicKey() {
    return null;
  }

  /** Algorithm identifier, e.g., {@code Ed25519}. */
  String algorithm();
}
